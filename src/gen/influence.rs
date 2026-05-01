use std::collections::{BTreeMap, BTreeSet};

/// Identifier for a civilisation in long-horizon world generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilisationId(pub u32);

/// Identifier for a settlement node in a civilisation influence graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SettlementId(pub u32);

/// Coarse world coordinate used by history generation.
///
/// This is deliberately independent from `TileCoord`/`ChunkCoord` while the
/// worldgen model is still being sketched. A later integration can decide
/// whether one influence cell maps to a chunk, a region, or a true map tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenCoord {
    pub x: i32,
    pub y: i32,
}

impl GenCoord {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    fn manhattan_distance(self, other: Self) -> u32 {
        self.x.abs_diff(other.x) + self.y.abs_diff(other.y)
    }

    fn neighbors(self) -> [Self; 4] {
        [
            Self::new(self.x - 1, self.y),
            Self::new(self.x + 1, self.y),
            Self::new(self.x, self.y - 1),
            Self::new(self.x, self.y + 1),
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenBounds {
    pub min_x: i32,
    pub min_y: i32,
    pub width: i32,
    pub height: i32,
}

impl GenBounds {
    pub const fn new(min_x: i32, min_y: i32, width: i32, height: i32) -> Self {
        Self {
            min_x,
            min_y,
            width,
            height,
        }
    }

    pub fn contains(self, coord: GenCoord) -> bool {
        coord.x >= self.min_x
            && coord.y >= self.min_y
            && coord.x < self.min_x + self.width
            && coord.y < self.min_y + self.height
    }
}

/// Persistent record for a people/polity participating in history generation.
#[derive(Clone, Debug, PartialEq)]
pub struct Civilisation {
    pub id: CivilisationId,
    pub name: String,
    pub vigor: f32,
}

/// A settlement is the durable node in the influence graph.
///
/// `footprint` may contain many coordinates because a city, camp, ruin, or
/// depot-town can occupy more than one eventual map tile. The graph edge list
/// is intentionally stored here: roads/paths can later be derived from the same
/// relationships that reinforce civilisational influence.
#[derive(Clone, Debug, PartialEq)]
pub struct Settlement {
    pub id: SettlementId,
    pub civilisation: CivilisationId,
    pub name: String,
    pub footprint: Vec<GenCoord>,
    pub linked_settlements: Vec<SettlementId>,
    pub strength: f32,
    pub status: SettlementStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettlementStatus {
    Active,
    Abandoned,
}

/// Current owner and strength of one influence cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InfluenceClaim {
    pub civilisation: CivilisationId,
    pub strength: f32,
    pub last_supported_year: i32,
}

/// Tunable knobs for one history tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InfluenceRules {
    pub settlement_projection_radius: u32,
    pub projection_decay_per_step: f32,
    pub held_tile_inertia: f32,
    pub connected_route_bonus: f32,
    pub growth_threshold: f32,
    pub takeover_margin: f32,
    pub unsupported_decay: f32,
    pub settlement_abandonment_share: f32,
    pub settlement_conversion_share: f32,
}

impl Default for InfluenceRules {
    fn default() -> Self {
        Self {
            settlement_projection_radius: 5,
            projection_decay_per_step: 0.18,
            held_tile_inertia: 0.35,
            connected_route_bonus: 0.70,
            growth_threshold: 0.30,
            takeover_margin: 0.20,
            unsupported_decay: 0.18,
            settlement_abandonment_share: 0.25,
            settlement_conversion_share: 0.55,
        }
    }
}

/// Pure worldgen state for experimenting with civilisational borders.
#[derive(Clone, Debug, PartialEq)]
pub struct InfluenceWorld {
    pub year: i32,
    pub bounds: GenBounds,
    pub civilisations: Vec<Civilisation>,
    pub settlements: Vec<Settlement>,
    pub claims: BTreeMap<GenCoord, InfluenceClaim>,
}

impl InfluenceWorld {
    pub fn new(bounds: GenBounds) -> Self {
        Self {
            year: 0,
            bounds,
            civilisations: Vec::new(),
            settlements: Vec::new(),
            claims: BTreeMap::new(),
        }
    }

    pub fn settlement(&self, id: SettlementId) -> Option<&Settlement> {
        self.settlements
            .iter()
            .find(|settlement| settlement.id == id)
    }

    pub fn claim_at(&self, coord: GenCoord) -> Option<InfluenceClaim> {
        self.claims.get(&coord).copied()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InfluenceReport {
    pub year: i32,
    pub claimed: usize,
    pub converted: usize,
    pub contracted: usize,
    pub abandoned_settlements: Vec<SettlementId>,
    pub converted_settlements: Vec<(SettlementId, CivilisationId, CivilisationId)>,
}

/// Advances civilisation influence by one coarse history year.
///
/// The algorithm has three phases: active settlements project pressure, border
/// cells resolve ownership, and then settlements react to the resulting local
/// control. It is pure Rust data transformation on purpose; ECS should consume
/// the generated world history later, not own the history simulation itself.
pub fn advance_influence_year(
    world: &InfluenceWorld,
    rules: InfluenceRules,
) -> (InfluenceWorld, InfluenceReport) {
    debug_assert!(rules.projection_decay_per_step >= 0.0);
    debug_assert!(rules.held_tile_inertia >= 0.0);
    debug_assert!(rules.growth_threshold >= 0.0);
    debug_assert!(rules.takeover_margin >= 0.0);

    let next_year = world.year + 1;
    let settlement_index = settlement_index(world);
    let civilisations = civilisation_vigor_index(world);
    let pressure = projected_pressure(world, &settlement_index, &civilisations, rules);
    let candidates = candidate_cells(world, &pressure);
    let mut claims = BTreeMap::new();
    let mut report = InfluenceReport {
        year: next_year,
        ..InfluenceReport::default()
    };

    for coord in candidates {
        if !world.bounds.contains(coord) {
            continue;
        }

        let current = world.claims.get(&coord).copied();
        let current_owner = current.map(|claim| claim.civilisation);
        let best = strongest_pressure(pressure.get(&coord), current_owner, rules.held_tile_inertia);

        match resolve_claim(coord, current, best, &pressure, rules, next_year) {
            ClaimResolution::Claimed(claim) => {
                if current.is_none() {
                    report.claimed += 1;
                } else if current_owner != Some(claim.civilisation) {
                    report.converted += 1;
                }
                claims.insert(coord, claim);
            }
            ClaimResolution::Contracted(claim) => {
                report.contracted += 1;
                claims.insert(coord, claim);
            }
            ClaimResolution::Lost => {
                if current.is_some() {
                    report.contracted += 1;
                }
            }
        }
    }

    protect_active_settlement_footprints(world, next_year, &pressure, &mut claims);

    let mut next = InfluenceWorld {
        year: next_year,
        bounds: world.bounds,
        civilisations: world.civilisations.clone(),
        settlements: world.settlements.clone(),
        claims,
    };
    update_settlements(&mut next, rules, &mut report);

    (next, report)
}

type PressureMap = BTreeMap<GenCoord, BTreeMap<CivilisationId, f32>>;

#[derive(Clone, Copy, Debug, PartialEq)]
struct PressureWinner {
    civilisation: CivilisationId,
    pressure: f32,
    runner_up: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ClaimResolution {
    Claimed(InfluenceClaim),
    Contracted(InfluenceClaim),
    Lost,
}

fn settlement_index(world: &InfluenceWorld) -> BTreeMap<SettlementId, &Settlement> {
    world
        .settlements
        .iter()
        .map(|settlement| (settlement.id, settlement))
        .collect()
}

fn civilisation_vigor_index(world: &InfluenceWorld) -> BTreeMap<CivilisationId, f32> {
    world
        .civilisations
        .iter()
        .map(|civilisation| (civilisation.id, civilisation.vigor.max(0.0)))
        .collect()
}

fn projected_pressure(
    world: &InfluenceWorld,
    settlement_index: &BTreeMap<SettlementId, &Settlement>,
    civilisations: &BTreeMap<CivilisationId, f32>,
    rules: InfluenceRules,
) -> PressureMap {
    let mut pressure = BTreeMap::new();

    for settlement in world
        .settlements
        .iter()
        .filter(|settlement| settlement.status == SettlementStatus::Active)
    {
        let vigor = civilisations
            .get(&settlement.civilisation)
            .copied()
            .unwrap_or(1.0);
        for coord in cells_within_radius(world.bounds, &settlement.footprint, rules) {
            let distance = distance_to_footprint(coord, &settlement.footprint);
            let projected =
                settlement.strength * vigor - distance as f32 * rules.projection_decay_per_step;
            add_pressure(
                &mut pressure,
                coord,
                settlement.civilisation,
                projected.max(0.0),
            );
        }

        for linked_id in &settlement.linked_settlements {
            if let Some(linked) = settlement_index.get(linked_id) {
                if linked.status == SettlementStatus::Active
                    && linked.civilisation == settlement.civilisation
                {
                    add_route_pressure(&mut pressure, world.bounds, settlement, linked, rules);
                }
            }
        }
    }

    pressure
}

fn cells_within_radius(
    bounds: GenBounds,
    footprint: &[GenCoord],
    rules: InfluenceRules,
) -> BTreeSet<GenCoord> {
    let mut cells = BTreeSet::new();
    let radius = i32::try_from(rules.settlement_projection_radius).unwrap_or(i32::MAX);
    for origin in footprint {
        for y in (origin.y - radius)..=(origin.y + radius) {
            for x in (origin.x - radius)..=(origin.x + radius) {
                let coord = GenCoord::new(x, y);
                if bounds.contains(coord)
                    && origin.manhattan_distance(coord) <= rules.settlement_projection_radius
                {
                    cells.insert(coord);
                }
            }
        }
    }
    cells
}

fn add_route_pressure(
    pressure: &mut PressureMap,
    bounds: GenBounds,
    a: &Settlement,
    b: &Settlement,
    rules: InfluenceRules,
) {
    let Some(start) = settlement_center(a) else {
        return;
    };
    let Some(end) = settlement_center(b) else {
        return;
    };

    for coord in manhattan_route(start, end) {
        if bounds.contains(coord) {
            add_pressure(
                pressure,
                coord,
                a.civilisation,
                rules.connected_route_bonus * a.strength.min(b.strength),
            );
        }
    }
}

fn add_pressure(
    pressure: &mut PressureMap,
    coord: GenCoord,
    civilisation: CivilisationId,
    value: f32,
) {
    if value <= 0.0 {
        return;
    }
    let civ_pressure = pressure
        .entry(coord)
        .or_default()
        .entry(civilisation)
        .or_default();
    *civ_pressure += value;
}

fn candidate_cells(world: &InfluenceWorld, pressure: &PressureMap) -> BTreeSet<GenCoord> {
    let mut candidates: BTreeSet<_> = pressure.keys().copied().collect();

    for coord in world.claims.keys().copied() {
        candidates.insert(coord);
        for neighbor in coord.neighbors() {
            if world.bounds.contains(neighbor) {
                candidates.insert(neighbor);
            }
        }
    }

    candidates
}

fn strongest_pressure(
    pressures: Option<&BTreeMap<CivilisationId, f32>>,
    current_owner: Option<CivilisationId>,
    held_tile_inertia: f32,
) -> Option<PressureWinner> {
    let mut winner: Option<PressureWinner> = None;

    for (civilisation, raw_pressure) in pressures.into_iter().flatten() {
        let pressure = if Some(*civilisation) == current_owner {
            *raw_pressure + held_tile_inertia
        } else {
            *raw_pressure
        };

        match winner {
            Some(current)
                if pressure <= current.pressure
                    || (pressure == current.pressure && *civilisation > current.civilisation) =>
            {
                winner = Some(PressureWinner {
                    runner_up: current.runner_up.max(pressure),
                    ..current
                });
            }
            Some(current) => {
                winner = Some(PressureWinner {
                    civilisation: *civilisation,
                    pressure,
                    runner_up: current.pressure.max(current.runner_up),
                });
            }
            None => {
                winner = Some(PressureWinner {
                    civilisation: *civilisation,
                    pressure,
                    runner_up: 0.0,
                });
            }
        }
    }

    winner
}

fn resolve_claim(
    coord: GenCoord,
    current: Option<InfluenceClaim>,
    winner: Option<PressureWinner>,
    pressure: &PressureMap,
    rules: InfluenceRules,
    year: i32,
) -> ClaimResolution {
    let Some(winner) = winner else {
        return decay_claim(current, rules);
    };

    debug_assert!(winner.pressure.is_finite());

    let contested_margin = winner.pressure - winner.runner_up;
    if current.is_none()
        && winner.pressure >= rules.growth_threshold
        && contested_margin >= rules.takeover_margin
    {
        return ClaimResolution::Claimed(InfluenceClaim {
            civilisation: winner.civilisation,
            strength: winner.pressure,
            last_supported_year: year,
        });
    }

    let Some(current_claim) = current else {
        return ClaimResolution::Lost;
    };

    if current_claim.civilisation == winner.civilisation {
        return ClaimResolution::Claimed(InfluenceClaim {
            civilisation: current_claim.civilisation,
            strength: current_claim.strength.max(winner.pressure),
            last_supported_year: year,
        });
    }

    let owner_pressure = pressure
        .get(&coord)
        .and_then(|pressures| pressures.get(&current_claim.civilisation))
        .copied()
        .unwrap_or(0.0)
        + rules.held_tile_inertia;

    if winner.pressure >= owner_pressure + rules.takeover_margin {
        ClaimResolution::Claimed(InfluenceClaim {
            civilisation: winner.civilisation,
            strength: winner.pressure,
            last_supported_year: year,
        })
    } else {
        ClaimResolution::Claimed(InfluenceClaim {
            civilisation: current_claim.civilisation,
            strength: current_claim.strength.max(owner_pressure),
            last_supported_year: year,
        })
    }
}

fn decay_claim(current: Option<InfluenceClaim>, rules: InfluenceRules) -> ClaimResolution {
    let Some(current) = current else {
        return ClaimResolution::Lost;
    };
    let strength = current.strength - rules.unsupported_decay;
    if strength > 0.0 {
        ClaimResolution::Contracted(InfluenceClaim {
            strength,
            ..current
        })
    } else {
        ClaimResolution::Lost
    }
}

fn protect_active_settlement_footprints(
    world: &InfluenceWorld,
    year: i32,
    pressure: &PressureMap,
    claims: &mut BTreeMap<GenCoord, InfluenceClaim>,
) {
    for settlement in world
        .settlements
        .iter()
        .filter(|settlement| settlement.status == SettlementStatus::Active)
    {
        for coord in &settlement.footprint {
            if !world.bounds.contains(*coord) {
                continue;
            }
            let strength = pressure
                .get(coord)
                .and_then(|pressures| pressures.get(&settlement.civilisation))
                .copied()
                .unwrap_or(settlement.strength);
            claims.insert(
                *coord,
                InfluenceClaim {
                    civilisation: settlement.civilisation,
                    strength,
                    last_supported_year: year,
                },
            );
        }
    }
}

fn update_settlements(
    world: &mut InfluenceWorld,
    rules: InfluenceRules,
    report: &mut InfluenceReport,
) {
    for index in 0..world.settlements.len() {
        if world.settlements[index].status == SettlementStatus::Abandoned {
            continue;
        }

        let footprint = world.settlements[index].footprint.clone();
        if footprint.is_empty() {
            world.settlements[index].status = SettlementStatus::Abandoned;
            report
                .abandoned_settlements
                .push(world.settlements[index].id);
            continue;
        }

        let own_civ = world.settlements[index].civilisation;
        let local = local_control_share(world, &footprint, own_civ, rules);

        if local.own_share < rules.settlement_conversion_share {
            if let Some(conqueror) = local.strongest_other {
                world.settlements[index].civilisation = conqueror;
                report.converted_settlements.push((
                    world.settlements[index].id,
                    own_civ,
                    conqueror,
                ));
            } else if local.own_share < rules.settlement_abandonment_share {
                world.settlements[index].status = SettlementStatus::Abandoned;
                report
                    .abandoned_settlements
                    .push(world.settlements[index].id);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LocalControl {
    own_share: f32,
    strongest_other: Option<CivilisationId>,
}

fn local_control_share(
    world: &InfluenceWorld,
    footprint: &[GenCoord],
    own_civ: CivilisationId,
    rules: InfluenceRules,
) -> LocalControl {
    let cells = cells_within_radius(world.bounds, footprint, rules);
    if cells.is_empty() {
        return LocalControl {
            own_share: 0.0,
            strongest_other: None,
        };
    }

    let mut own = 0usize;
    let mut other_counts = BTreeMap::<CivilisationId, usize>::new();
    for cell in &cells {
        if let Some(claim) = world.claims.get(cell) {
            if claim.civilisation == own_civ {
                own += 1;
            } else {
                *other_counts.entry(claim.civilisation).or_default() += 1;
            }
        }
    }

    LocalControl {
        own_share: own as f32 / cells.len() as f32,
        strongest_other: other_counts
            .into_iter()
            .max_by_key(|(civilisation, count)| (*count, std::cmp::Reverse(*civilisation)))
            .map(|(civilisation, _)| civilisation),
    }
}

fn distance_to_footprint(coord: GenCoord, footprint: &[GenCoord]) -> u32 {
    footprint
        .iter()
        .map(|tile| coord.manhattan_distance(*tile))
        .min()
        .unwrap_or(u32::MAX)
}

fn settlement_center(settlement: &Settlement) -> Option<GenCoord> {
    if settlement.footprint.is_empty() {
        return None;
    }

    let mut sum_x = 0i64;
    let mut sum_y = 0i64;
    for coord in &settlement.footprint {
        sum_x += i64::from(coord.x);
        sum_y += i64::from(coord.y);
    }
    let count = i64::try_from(settlement.footprint.len()).ok()?;
    Some(GenCoord::new(
        i32::try_from(sum_x / count).ok()?,
        i32::try_from(sum_y / count).ok()?,
    ))
}

fn manhattan_route(start: GenCoord, end: GenCoord) -> Vec<GenCoord> {
    let mut route = Vec::new();
    let mut current = start;
    route.push(current);

    while current.x != end.x {
        current.x += (end.x - current.x).signum();
        route.push(current);
    }

    while current.y != end.y {
        current.y += (end.y - current.y).signum();
        route.push(current);
    }

    route
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: CivilisationId = CivilisationId(1);
    const BLUE: CivilisationId = CivilisationId(2);

    fn civ(id: CivilisationId, name: &str, vigor: f32) -> Civilisation {
        Civilisation {
            id,
            name: name.to_owned(),
            vigor,
        }
    }

    fn settlement(
        id: u32,
        civilisation: CivilisationId,
        x: i32,
        y: i32,
        strength: f32,
    ) -> Settlement {
        Settlement {
            id: SettlementId(id),
            civilisation,
            name: format!("settlement-{id}"),
            footprint: vec![GenCoord::new(x, y)],
            linked_settlements: Vec::new(),
            strength,
            status: SettlementStatus::Active,
        }
    }

    fn world_with_settlements(settlements: Vec<Settlement>) -> InfluenceWorld {
        InfluenceWorld {
            year: 0,
            bounds: GenBounds::new(0, 0, 16, 8),
            civilisations: vec![civ(RED, "Red", 1.0), civ(BLUE, "Blue", 1.0)],
            settlements,
            claims: BTreeMap::new(),
        }
    }

    #[test]
    fn active_settlement_grows_into_empty_space() {
        let world = world_with_settlements(vec![settlement(1, RED, 3, 3, 1.0)]);
        let (next, report) = advance_influence_year(&world, InfluenceRules::default());

        assert!(report.claimed > 1);
        assert_eq!(
            next.claim_at(GenCoord::new(3, 3))
                .map(|claim| claim.civilisation),
            Some(RED)
        );
        assert_eq!(
            next.claim_at(GenCoord::new(4, 3))
                .map(|claim| claim.civilisation),
            Some(RED)
        );
    }

    #[test]
    fn stronger_neighbour_converts_border_claims() {
        let mut world = world_with_settlements(vec![
            settlement(1, RED, 3, 3, 0.65),
            settlement(2, BLUE, 7, 3, 1.25),
        ]);
        world.claims.insert(
            GenCoord::new(5, 3),
            InfluenceClaim {
                civilisation: RED,
                strength: 0.5,
                last_supported_year: 0,
            },
        );

        let (next, report) = advance_influence_year(&world, InfluenceRules::default());

        assert!(report.converted > 0);
        assert_eq!(
            next.claim_at(GenCoord::new(5, 3))
                .map(|claim| claim.civilisation),
            Some(BLUE)
        );
    }

    #[test]
    fn unsupported_claims_decay_and_disappear() {
        let mut world = world_with_settlements(Vec::new());
        world.claims.insert(
            GenCoord::new(2, 2),
            InfluenceClaim {
                civilisation: RED,
                strength: 0.10,
                last_supported_year: 0,
            },
        );

        let (next, report) = advance_influence_year(&world, InfluenceRules::default());

        assert_eq!(report.contracted, 1);
        assert_eq!(next.claim_at(GenCoord::new(2, 2)), None);
    }

    #[test]
    fn linked_settlements_reinforce_route_between_them() {
        let mut west = settlement(1, RED, 2, 3, 0.55);
        west.linked_settlements.push(SettlementId(2));
        let east = settlement(2, RED, 8, 3, 0.55);
        let world = world_with_settlements(vec![west, east]);

        let (next, _) = advance_influence_year(&world, InfluenceRules::default());

        assert_eq!(
            next.claim_at(GenCoord::new(5, 3))
                .map(|claim| claim.civilisation),
            Some(RED)
        );
    }

    #[test]
    fn settlement_with_lost_hinterland_converts_to_local_power() {
        let mut world = world_with_settlements(vec![
            settlement(1, RED, 5, 3, 0.15),
            settlement(2, BLUE, 6, 3, 1.4),
        ]);
        for y in 1..=5 {
            for x in 4..=8 {
                world.claims.insert(
                    GenCoord::new(x, y),
                    InfluenceClaim {
                        civilisation: BLUE,
                        strength: 1.0,
                        last_supported_year: 0,
                    },
                );
            }
        }

        let (next, report) = advance_influence_year(&world, InfluenceRules::default());
        let converted = report
            .converted_settlements
            .iter()
            .any(|(settlement, from, to)| {
                *settlement == SettlementId(1) && *from == RED && *to == BLUE
            });

        assert!(converted);
        assert_eq!(
            next.settlement(SettlementId(1))
                .map(|town| town.civilisation),
            Some(BLUE)
        );
    }

    #[test]
    fn settlement_footprints_can_cover_multiple_cells() {
        let mut town = settlement(1, RED, 3, 3, 1.0);
        town.footprint.push(GenCoord::new(4, 3));
        let world = world_with_settlements(vec![town]);

        let (next, _) = advance_influence_year(&world, InfluenceRules::default());

        assert_eq!(
            next.claim_at(GenCoord::new(3, 3))
                .map(|claim| claim.civilisation),
            Some(RED)
        );
        assert_eq!(
            next.claim_at(GenCoord::new(4, 3))
                .map(|claim| claim.civilisation),
            Some(RED)
        );
    }
}

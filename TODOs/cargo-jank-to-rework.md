A lot of it is boring mechanical stuff, and I think I can slog through a meaningful slice without us needing to over-design it first.

I’d take a piecemeal approach, but not because the work is conceptually hard. More because cargo touches movement, NPC jobs, inventory, rendering, headless snapshots, and tests. One giant “purify cargo” patch would be annoying to review and easy to subtly break.

My preferred order:

1. **Remove cached `Cargo.current_weight`**
   Keep actor capacity, but stop storing load. Replace `Cargo { current_weight, max_weight }` with something like `CarryCapacity { max_weight }` or `CargoCapacity { max_weight }`, and derive load at read sites.
   
   This is the big jank reducer. Movement/HUD/headless/porter debug can call a derived-load helper or use a system param/query helper.

!== DONE! ==!

2. **Make pickup/drop generic over `Item + CargoStats`**
   The core request systems are already close. `PickUpRequest` and `DropRequest` are item-shaped. The remaining parcel-ness is mostly caller/query filtering and `ParcelState` maintenance.
   
   Player pickup should find loose generic cargo at the actor position, not only `CargoParcel`.

3. **Split parcel delivery state from carry state**
   `ParcelState::CarriedBy(actor)` is redundant now. Carry state should be `CarriedBy` / `ContainedIn`; parcel state should probably only be delivery/job lifecycle, maybe:
   
   ```rust
   enum ParcelState {
       Loose,
       AssignedTo(Entity),
       Delivered,
   }
   ```
   
   Or even split that further later into `DeliveryJob`/`Deliverable`.

4. **Replace `CargoParcel { weight }`**
   Either make it a zero-sized marker `Parcel`, or turn it into delivery-specific metadata. Weight should live in `CargoStats`.

5. **Make NPC jobs item/cargo aware**
   The porter job can still be parcel-oriented for delivery gameplay, but the pickup mechanics should not care. If we want NPCs to pick up non-parcel cargo for future jobs, the job should point at an `item`, not a `parcel`.

I’d probably do steps 1 and 2 together only if the edits stay clean. If step 1 turns into a bunch of call-site churn, I’d stop there, verify, then do generic pickup/drop next.

So: yes, I can handle a good chunk of this as mechanical cleanup. The one place I’d be careful is naming/design around `ParcelState`, because that’s where domain meaning lives. Everything else is mostly moving the code toward the model we already agreed on: relationships are truth, load is derived, parcels are just a specific kind of cargo.

## TODOs

1. Fix the failing unit test: `a_gravity_assisted_voyage_flown_live_passes_its_planet_and_lands`
2. Introduce the new algorithm to perform the following:
  1. Calculate the initial optimal trajectory.
  2. As the spaceship transitions, use Dijstras algorithm to find the optimal path and re-calculate as the rocket moves along.
3. There is a bug that whenever you speed up or slow down the experiment, the trajectory that was calculated gets thrown off.
4. ~~Implement the indexing technique, to simplify the search technique.~~

## Side TODOs

1. Call Astound to cancel the subscription.
routes.draw do
  match ':controller/:action/:id'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `get` instead of `match` to define a route.
end

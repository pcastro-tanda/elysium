routes.draw do
  match "#{resource}/:action/:id", via: [:put]
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `put` instead of `match` to define a route.
end

routes.draw do
  match 'photos/:id' => 'photos#show', via: :get
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `get` instead of `match` to define a route.
end

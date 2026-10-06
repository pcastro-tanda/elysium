routes.draw do
  match 'photos/:id', to: 'photos#update', via: [:put, :patch]
end

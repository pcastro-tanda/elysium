routes.draw do
  match 'photos/:id', to: 'photos#show', via: :all
end

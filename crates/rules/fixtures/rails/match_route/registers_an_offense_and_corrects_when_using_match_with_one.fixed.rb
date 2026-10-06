routes.draw do
  get 'photos/:id', to: 'photos#show'
end

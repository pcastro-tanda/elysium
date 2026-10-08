Rails.application.routes.draw do
  get '/users', to: 'users#index'
  get '/other_path/users', to: 'users#index'
  get '/another_path/users', to: 'users#index'
end

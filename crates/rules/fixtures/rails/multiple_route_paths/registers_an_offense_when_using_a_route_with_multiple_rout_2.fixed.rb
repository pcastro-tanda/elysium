Rails.application.routes.draw do
  get '/users', **options
  get '/other_path/users', **options
  get '/another_path/users', **options
end

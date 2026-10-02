Rails.application.routes.draw do
  get '/users', '/other_path/users', '/another_path/users', **options
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use separate routes instead of combining multiple route paths in a single route.
end

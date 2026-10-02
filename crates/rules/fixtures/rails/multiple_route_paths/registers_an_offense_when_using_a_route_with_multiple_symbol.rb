Rails.application.routes.draw do
  get :resend, :generate_new_password
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use separate routes instead of combining multiple route paths in a single route.
end

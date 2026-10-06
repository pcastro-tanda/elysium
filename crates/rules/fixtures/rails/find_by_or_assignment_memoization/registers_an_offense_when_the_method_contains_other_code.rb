def current_user
  @current_user ||= User.find_by(id: session[:user_id])
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid memoizing `find_by` results with `||=`.

  @current_user.do_something!
end

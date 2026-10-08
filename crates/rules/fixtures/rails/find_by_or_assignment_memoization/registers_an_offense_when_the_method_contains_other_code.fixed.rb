def current_user
  if defined?(@current_user)
  @current_user
else
  @current_user = User.find_by(id: session[:user_id])
end

  @current_user.do_something!
end

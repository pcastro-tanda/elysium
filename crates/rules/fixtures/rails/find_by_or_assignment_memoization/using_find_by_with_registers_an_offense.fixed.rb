if defined?(@current_user)
  @current_user
else
  @current_user = User.find_by(id: session[:user_id])
end

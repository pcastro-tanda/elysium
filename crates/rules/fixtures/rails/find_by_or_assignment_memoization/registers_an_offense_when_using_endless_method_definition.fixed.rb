def current_user(arg) 
return @current_user if defined?(@current_user)

@current_user = User.find_by(id: session[:user_id])
end

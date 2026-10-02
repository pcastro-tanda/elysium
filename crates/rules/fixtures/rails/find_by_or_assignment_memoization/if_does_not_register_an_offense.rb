@current_user ||= User.find_by(id: session[:user_id]) if session[:user_id]

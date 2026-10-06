@current_user ||= User.find_by(id: session[:user_id]) unless session[:user_id].nil?

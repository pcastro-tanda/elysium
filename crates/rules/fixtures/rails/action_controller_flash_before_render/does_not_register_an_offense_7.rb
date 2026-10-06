class HomeController < ApplicationController
  def create
    if remote_request? || sandbox?
      if current_user.nil?
        render :index
      else
        head :forbidden
      end
    elsif current_user.nil?
      redirect_to sign_in_path
    else
      flash[:alert] = 'msg'
      if request.referer.present?
        redirect_to(request.referer)
      else
        redirect_to(root_path)
      end
    end
  end
end

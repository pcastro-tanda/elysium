class HomeController < ActionController::Base
  def create
    if condition
      flash[:alert] = "msg"
    end

    redirect_to :index
  end
end

class HomeController < ActionController::Base
  def create
    if condition
      do_something
      flash[:alert] = "msg"
    end

    redirect_to :index
  end
end

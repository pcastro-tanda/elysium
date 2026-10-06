class HomeController < ActionController::Base
  def create
    if condition
      flash[:alert] = "msg"
      redirect_to :index

      return
    end

    render :index
  end
end

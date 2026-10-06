class HomeController < ApplicationController
  def create
    if condition
      flash[:alert] = "msg"
      return redirect_to "https://www.example.com/"
    end
    render :index
  end
end

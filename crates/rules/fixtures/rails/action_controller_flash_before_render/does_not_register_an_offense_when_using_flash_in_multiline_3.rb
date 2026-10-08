class HomeController < ApplicationController
  def create
    begin
      do_something
      flash[:alert] = "msg in begin"
    rescue
      flash[:alert] = "msg in rescue"
    end

    redirect_to :index
  end
end

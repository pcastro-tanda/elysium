class HomeController < ActionController::Base
  def create
    if condition
      do_something
      flash[:alert] = "msg"
      ^^^^^ Use `flash.now` before `render`.
    end

    render :index
  end
end

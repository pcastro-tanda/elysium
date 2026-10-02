class HomeController < ::ActionController::Base
  def create
    flash[:alert] = "msg"
    ^^^^^ Use `flash.now` before `render`.
  end
end

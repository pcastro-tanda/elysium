class NonController < ApplicationRecord
  def create
    flash[:alert] = "msg"
    render :index
  end
end

class NonController < ApplicationRecord
  before_action do
    flash[:alert] = "msg"
    render :index
  end
end

class NonController < ApplicationRecord
  before_action do
    flash[:notice] = 'hello'
  end.do_something
end

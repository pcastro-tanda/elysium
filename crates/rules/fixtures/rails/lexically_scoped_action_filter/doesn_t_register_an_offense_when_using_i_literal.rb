class FooController < ApplicationController
  before_action :foo, except: %I[index show]

  def index
  end

  def show
  end
end

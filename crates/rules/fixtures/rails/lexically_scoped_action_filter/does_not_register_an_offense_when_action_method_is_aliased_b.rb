class FooController < ApplicationController
  before_action :authorize!, only: %i[index show]

  def index
  end
  alias_method :show, :index

  private

  def authorize!
  end
end

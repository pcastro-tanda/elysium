class FooController < ApplicationController
  before_action :authorize!, only: %i[foo show]
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `foo` is not explicitly defined on the class.

  def index
  end
  alias_method :show, :index

  private

  def authorize!
  end
end

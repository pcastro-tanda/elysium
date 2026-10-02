class FooController < ApplicationController
  before_action :authorize!, only: %i[index show]

  def index
  end
  alias show index

  private

  def authorize!
  end
end

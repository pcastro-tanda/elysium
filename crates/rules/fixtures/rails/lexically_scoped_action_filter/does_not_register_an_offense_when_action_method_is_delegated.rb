class FooController < ApplicationController
  before_action :authorize!, only: %i[index show]

  delegate :index, :show, to: :foo
end

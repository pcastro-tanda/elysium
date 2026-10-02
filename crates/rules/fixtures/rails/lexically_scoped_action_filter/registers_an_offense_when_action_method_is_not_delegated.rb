class FooController < ApplicationController
  before_action :authorize!, only: %i[foo show]
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `foo` is not explicitly defined on the class.

  delegate :show, to: :bar
end

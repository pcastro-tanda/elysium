module RotationCoordinatorTests
  extend ActiveSupport::Concern

  included do
    teardown do
      @coordinator = { a: "A" }
                     ^^^^^^^^^^ Freeze mutable objects assigned to class instance variables.
    end
  end
end

module RotationCoordinatorTests
  extend ActiveSupport::Concern

  included do
    setup do
      @coordinator = { a: "A" }
                     ^^^^^^^^^^ Freeze mutable objects assigned to class instance variables.
    end
  end
end

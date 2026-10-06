class X < ApplicationRecord
  def destroy
  ^^^^^^^^^^^ Use `before_destroy`, `around_destroy`, or `after_destroy` callbacks instead of overriding the Active Record method `destroy`.
    super
  end
end

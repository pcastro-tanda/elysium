class X < ActiveModel::Base
  module_function

  def update
  ^^^^^^^^^^ Use `before_update`, `around_update`, or `after_update` callbacks instead of overriding the Active Record method `update`.
    super
  end
end

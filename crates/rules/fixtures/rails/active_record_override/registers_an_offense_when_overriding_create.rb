class X < ApplicationRecord
  def create
  ^^^^^^^^^^ Use `before_create`, `around_create`, or `after_create` callbacks instead of overriding the Active Record method `create`.
    super
  end
end

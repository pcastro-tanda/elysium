module FooMixin
  extend ActiveSupport::Concern

  included do
    before_action proc { authenticate }, only: :foo
  end

  def foo; end
end

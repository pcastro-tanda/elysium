module Foo
  def self.included(base)
    base.has_many :bazs, dependent: :destroy
  end
end

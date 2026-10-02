class Picture < ApplicationRecord
  belongs_to :imageable, -> () { where(bar: true) }, polymorphic: true
end

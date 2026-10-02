class Model < ApplicationRecord
  model.errors.where(:title).each { |error| do_something(error)  }
end

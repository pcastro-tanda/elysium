class C < ApplicationRecord
  all.each { |u| u.x }
      ^^^^ Use `find_each` instead of `each`.
end

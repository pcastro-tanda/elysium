class C < ActiveRecord::Base
  all.each { |u| u.x }
      ^^^^ Use `find_each` instead of `each`.
end

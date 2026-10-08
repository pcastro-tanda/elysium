class C < ::ActiveRecord::Base
  def do_something
    find_by_name(name)
    ^^^^^^^^^^^^^^^^^^ Use `find_by` instead of dynamic `find_by_name`.
  end
end

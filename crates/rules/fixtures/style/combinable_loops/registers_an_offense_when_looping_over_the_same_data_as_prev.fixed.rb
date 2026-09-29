items.each { |item| do_something(item)
do_something_else(item, arg) }

items.each_with_index { |item| do_something(item)
do_something_else(item, arg) }

items.reverse_each { |item| do_something(item)
do_something_else(item, arg) }

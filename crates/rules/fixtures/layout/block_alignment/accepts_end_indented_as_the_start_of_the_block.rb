       my_object.chaining_this_very_long_method(with_a_parameter)
           .and_one_with_a_block do
         do_something
       end
# Other variant:
       my_object.chaining_this_very_long_method(
           with_a_parameter).and_one_with_a_block do
         do_something
       end

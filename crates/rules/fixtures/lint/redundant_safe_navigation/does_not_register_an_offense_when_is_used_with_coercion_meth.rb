foo&.to_s || 'Default string'
foo&.to_i || 1
do_something if foo&.to_d

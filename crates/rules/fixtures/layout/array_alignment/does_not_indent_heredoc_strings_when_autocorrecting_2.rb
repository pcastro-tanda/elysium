var = [
  { :type => 'something',
    :sql => <<EOF
Select something
from atable
EOF
  },
 { :type => 'something',
 ^^^^^^^^^^^^^^^^^^^^^^^ Use one level of indentation for elements following the first line of a multi-line array.
   :sql => <<EOF
Select something
from atable
EOF
 }
]

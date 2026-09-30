var = [
       { :type => 'something',
         :sql => <<EOF
Select something
from atable
EOF
       },
      { :type => 'something',
      ^^^^^^^^^^^^^^^^^^^^^^^ Align the elements of an array literal if they span more than one line.
        :sql => <<EOF
Select something
from atable
EOF
      }
]

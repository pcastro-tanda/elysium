User.where.not('name = ?', 'Gabe')
           ^^^^^^^^^^^^^^^^^^^^^^^ Use `not(name: 'Gabe')` instead of manually constructing SQL.

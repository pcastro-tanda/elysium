a || if b.present?
     ^^^^^^^^^^^^^ Use `b.presence || c` instead of `if b.present? ... end`.
       b
     else
       c
     end

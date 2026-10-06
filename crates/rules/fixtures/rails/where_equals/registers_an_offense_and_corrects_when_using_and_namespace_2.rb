Course.where(['enrollments.student_id = ?', student.id])
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `where(enrollments: { student_id: student.id })` instead of manually constructing SQL.

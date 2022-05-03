#pragma once

#include "ui_progressstatus.h"

class ProgressStatus: public QWidget
{
  Q_OBJECT

 public:
  explicit ProgressStatus(QWidget *parent = nullptr);

  void setLabel(const QString& text);
  void addProgressMaximum(const uint64_t val);
  void addProgress(const uint64_t val);

 private:
  Ui::ProgressStatus ui;
};

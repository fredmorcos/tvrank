#include "progressstatus.h"

ProgressStatus::ProgressStatus(QWidget *parent): QWidget(parent)
{
  ui.setupUi(this);
}

void ProgressStatus::setLabel(const QString& text)
{
  ui.label->setText(text);
}

void ProgressStatus::addProgressMaximum(const uint64_t val)
{
  ui.progress->setMaximum(ui.progress->maximum() + val);
}

void ProgressStatus::addProgress(const uint64_t val)
{
  ui.progress->setValue(ui.progress->value() + val);
}

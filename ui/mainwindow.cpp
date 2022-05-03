#include "mainwindow.h"
#include "./ui_mainwindow.h"
#include "config.h"
#include "titlesmodel.h"
#include "tvrankservice.h"
#include <memory>
#include <QtWidgets>

MainWindow::MainWindow(TitlesModel& moviesModel, TitlesModel& seriesModel, QWidget *parent):
 QMainWindow(parent),
 ui(new Ui::MainWindow),
 progressStatusUi(new ProgressStatus),
 moviesModel(moviesModel),
 seriesModel(seriesModel)
{
  ui->setupUi(this);

  connect(ui->actionGo, &QAction::triggered, this, &MainWindow::doSearch);
  connect(ui->actionAbout, &QAction::triggered, this, &MainWindow::showAbout);
  connect(ui->searchEdit, &QLineEdit::textChanged, this, &MainWindow::searchEditTextChanged);

  statusBar()->addWidget(progressStatusUi);
}

MainWindow::~MainWindow()
{
  delete progressStatusUi;
  delete ui;
}

void MainWindow::tvrankServiceFailed(const enum TVrankServiceError err)
{
  QString errMsg {"Error initializing TVrank service: "};

  switch (err) {
    case TVrankServiceError::InvalidCacheDirString:
      errMsg.append("Invalid cache directory string");
      break;
    case TVrankServiceError::ErrorCreatingService:
      errMsg.append("Cannot create service");
      break;
  }

  statusBar()->removeWidget(progressStatusUi);
  QMessageBox::critical(this, "TVrank Error", errMsg);
  qApp->exit(1);
}

void MainWindow::tvrankServiceSucceeded(const TVrankService& service)
{
  moviesModel.setService(service.service);
  seriesModel.setService(service.service);

  ui->moviesTreeView->setModel(&moviesModel);
  ui->seriesTreeView->setModel(&seriesModel);

  size_t nMovies = 0;
  size_t nSeries = 0;
  tvrank_service_entries_count(service.service, &nMovies, &nSeries);

  statusBar()->removeWidget(progressStatusUi);
  statusBar()->showMessage(QString("Total of %1 movies and %2 series").arg(nMovies).arg(nSeries));
}

void MainWindow::tvrankServiceContentLen(const uint64_t len)
{
  progressStatusUi->addProgressMaximum(len);
}

void MainWindow::tvrankServiceProgress(const uint64_t delta)
{
  progressStatusUi->addProgress(delta);
}

void MainWindow::doSearch()
{
  ui->searchEdit->clear();
}

void MainWindow::showAbout()
{
  const auto about
      = QString("<p>"
                "<a href=\"%2\">%1</a> version <b>%3</b>"
                "<br/>"
                "Licensed under the <a href=\"%4\">MIT license</a>"
                "<br/>"
                "Copyright &#169; 2021-2022 <a href=\"mailto:%5\">Fred Morcos</a>"
                "<br/>"
                "Report problems or requests <a href=\"%6\">at the public issue tracker</a>"
                "</p>"
                "<p>"
                "<b>Provide the information below when reporting problems:</b>"
                "<br/><br/>"
                "<b>Build Timestamp:</b> %7"
                "<br/>"
                "<b>Platform:</b> %8"
                "<br/>"
                "<b>Architecture:</b> %9"
                "<br/>"
                "<b>Build Type:</b> %10"
                "<br/>"
                "<b>Compiler:</b> %11"
                "<br/>"
                "<b>Compiler Version:</b> %12"
                "<br/>"
                "<b>Debug Flags:</b> %13"
                "<br/>"
                "<b>Release Flags:</b> %14"
                "<br/>"
                "<b>CMake Version:</b> %15"
                "<br/>"
                "</p>")
            .arg(qApp->applicationName())
            .arg("https://github.com/fredmorcos/tvrank")
            .arg(qApp->applicationVersion())
            .arg("https://github.com/fredmorcos/tvrank/blob/main/LICENSE")
            .arg("fm+TVrank@fredmorcos.com")
            .arg("https://github.com/fredmorcos/tvrank/issues")
            .arg(TVRANK_BUILD_TIMESTAMP)
            .arg(TVRANK_PLATFORM)
            .arg(TVRANK_ARCH)
            .arg(TVRANK_BUILD_TYPE)
            .arg(TVRANK_COMPILER_ID)
            .arg(TVRANK_COMPILER_VERSION)
            .arg(TVRANK_FLAGS_DEBUG)
            .arg(TVRANK_FLAGS_RELEASE)
            .arg(TVRANK_CMAKE_VERSION);
  QMessageBox::about(this, qApp->applicationName(), about);
}

void MainWindow::searchEditTextChanged()
{
  const bool enableSearchAction = !ui->searchEdit->text().isEmpty();
  ui->actionGo->setEnabled(enableSearchAction);
  ui->searchButton->setEnabled(enableSearchAction);
}
